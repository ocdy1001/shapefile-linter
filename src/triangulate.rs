use crate::data::{PolygonZ,StretchableBB};
use dlv_list::*;
use crate::data::*;
use crate::logger::*;
use std::cmp::Ordering;
use std::ops::{Add,Sub,Div,Mul};
use bin_buffer::*;
use crate::compress::*;
use std::convert::TryFrom;

#[derive(Clone)]
pub struct PolyTriangle<T>{
    vertices: Vec<(T,T)>,
    indices: Vec<u16>,
    style: usize,
}

impl<T: Bufferable + Clone> Bufferable for PolyTriangle<T>{
    fn into_buffer(self, buf: &mut Buffer){
        self.vertices.into_buffer(buf);
        self.indices.into_buffer(buf);
        self.style.into_buffer(buf);
    }

    fn copy_into_buffer(&self, buf: &mut Buffer){
        self.clone().into_buffer(buf);
    }

    fn from_buffer(buf: &mut ReadBuffer) -> Option<Self>{
        let vertices = Vec::<(T,T)>::from_buffer(buf)?;
        let indices = Vec::<u16>::from_buffer(buf)?;
        let style = usize::from_buffer(buf)?;
        Some(Self{
            vertices,
            indices,
            style,
        })
    }
}
#[derive(Clone,PartialEq)]
struct PolyPoint<T>{
    point: P3<T>,
    reflex: bool,
    ear: bool,
    index: u16
}

pub fn test(){
    let mut polyzs = Vec::new();

    polyzs.push(PolygonZ{
        inners: vec![
            vec![
                (9.0,2.0,0.0),
                (9.0,8.0,0.0),
                (5.0,8.0,0.0),
                (5.0,2.0,0.0),
            ],
            vec![
                (3.0,4.0,0.0),
                (2.5,5.0,0.0),
                (2.0,4.0,0.0),
            ],
            vec![
                (7.5,4.0,0.0),
                (7.0,5.0,0.0),
                (6.5,4.0,0.0),
            ]
        ],
        outers: vec![
            vec![
                (6.0,3.0,0.0),
                (6.0,7.0,0.0),
                (8.0,7.0,0.0),
                (8.0,3.0,0.0)
            ],
            vec![
                (0.0,0.0,0.0),
                (0.0,10.0,0.0),
                (10.0,10.0,0.0),
                (10.0,0.0,0.0)
            ]
        ],
        bb: ((0.0,0.0,0.0),(0.0,0.0,0.0)),
        style: 0,
    });


    let mut logger = Logger::default();
    let res = triangulate(polyzs, &mut logger);

    for polyt in res{
        print!("vertices: ");
        for vert in polyt.vertices{print!("({},{}),",vert.0,vert.1)}
        println!();
        print!("indices: ");
        let mut i = 0;
        for index in polyt.indices{
            if i == 0 {print!("),(")}
            print!("{},",index);
            i = (i+1)%3;
        }
        println!();
    }
}

pub fn triangulate<T>(polyzs: Vec<PolygonZ<T>>, logger: &mut Logger) -> Vec<PolyTriangle<T>>
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + Copy + Default + MinMax + std::fmt::Display,
    u8: Into<T>,
    T: Into<f64> + FromF64 + std::fmt::Debug
{
    let mut res = Vec::new();
    let mut skipped = 0;
    let polyzs = clean_polyzs(polyzs);
    for mut polygon in polyzs{
        let style = polygon.style;
        fix_order(&mut polygon);
        let grouped_polygons = group_polygons(polygon, &mut skipped);

        for (mut outer,inners) in grouped_polygons{
            let mut vertices = merge_inner(&mut outer, inners);
            if vertices.is_empty() { continue; }
            vertices.dedup();
            if vertices[0] == vertices[vertices.len()-1]{vertices.pop();}
            let cur_indices = if let Some(x) = make_indices(&vertices, logger)
            { x } else {  continue; };

            let mut p2vertices = Vec::new();
            for (x,y,_) in vertices{
                p2vertices.push((x,y));
            }
            res.push(PolyTriangle{
                vertices: p2vertices,
                indices: cur_indices,
                style: style,
            });
        }
    }
    if skipped > 0 { println!("Skipped {} inner polygons.", skipped); }
    res
}

fn clean_polyzs<T: Copy + PartialEq + MinMax + Default>
    (polyzs: Vec<PolygonZ<T>>) -> Vec<PolygonZ<T>>{
    // input need to have length > 0
    fn clean_poly<T: Copy + PartialEq>(poly: Vec<T>) -> Vec<T>{
        let mut new = Vec::new();
        let mut last = poly[0];
        new.push(last);
        for p in poly.into_iter().skip(1){
            if last == p { continue; }
            last = p;
            new.push(p);
        }
        new
    }
    let mut npolyzs = Vec::new();
    for polyz in polyzs.into_iter(){
        let mut nouters = Vec::new();
        for outer in polyz.outers.into_iter(){
            if outer.is_empty() { continue; }
            nouters.push(clean_poly(outer));
        }
        let mut ninners = Vec::new();
        for inner in polyz.inners.into_iter(){
            if inner.is_empty() { continue; }
            ninners.push(clean_poly(inner));
        }
        let d = T::default();
        let mut npolyz = PolygonZ{
            outers: nouters,
            inners: ninners,
            bb: ((d,d,d),(d,d,d)),
            style: polyz.style,
        };
        npolyz.stretch_bb();
        npolyzs.push(npolyz);
    }
    npolyzs
}

fn fix_order<T>(polygon: &mut PolygonZ<T>)
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + Copy,
    T: Into<f64>
{
    for outer in &mut polygon.outers{
        if !is_clockwise(&outer){
            outer.reverse();
        }
    }
    for inner in &mut polygon.inners{
        if is_clockwise(&inner){
            inner.reverse();
        }
    }
}

fn is_clockwise<T>(ring: &Vec<P3<T>>) -> bool
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + Copy,
    T: Into<f64>
{
    let mut sum = 0.0;
    for (i,p0) in ring.iter().enumerate(){
        let p1 = ring[(i+1)%ring.len()];
        let x0 = p0.0.into();
        let y0 = p0.1.into();
        let x1 = p1.0.into();
        let y1 = p1.1.into();
        sum += (x1-x0)*(y1+y0);
    }
    return sum > 0.0;
}

fn group_polygons<T>(polygon: PolygonZ<T>, skipped: &mut i64) -> Vec<(Vec<P3<T>>, Vvec<P3<T>>)>
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + Copy + std::fmt::Display,
    u8: Into<T>,
    T: Into<f64>
{
    //polygon polygon.outers[i] is inside inside[i] other polygons
    let mut inside = Vec::new();

    let mut grouped_inners = Vec::new();

    for outer in &polygon.outers{
        let mut count = 0;
        for other_outer in &polygon.outers{
            if outer == other_outer {continue}
            if is_inside_polygon(other_outer.to_vec(), outer[0]){
                count+=1;
            }
        }
        inside.push(count);

        grouped_inners.push(Vec::new());
    }

    for inner in polygon.inners{
        //find outer polygon that's inside the most other outer polygons
        //this is the 'most inner outer polygon', in a way, so this
        //inner ring belongs to that
        let mut max = -1;
        let mut max_index = 0;
        for (i,outer) in polygon.outers.iter().enumerate(){
            if is_inside_polygon(outer.to_vec(), inner[0]) && inside[i] > max  {
                max = inside[i];
                max_index = i;
            }
        }

        if max == -1{
            *skipped += 1;

            /*for (i,outer) in polygon.outers.iter().enumerate(){
                print_poly_matplotlib(&outer, "outer".to_owned() + &i.to_string());
            }
            print_poly_matplotlib(&inner, "inner".to_string());*/
        }

        grouped_inners[max_index].push(inner);
    }
    polygon.outers.into_iter().zip(grouped_inners).collect()
}

fn print_poly_matplotlib<T>(poly: &Vec<P3<T>>, name: String)
where
    T: std::fmt::Display
{
    print!("{}Verts = [", name);
    for p in poly{
        print!("({},{}),", p.0, p.1);
    }
    print!("(0,0),");
    println!("]");

    print!("{}Codes = [", name);
    for (i,p) in poly.iter().enumerate(){
        if i == 0 {print!("Path.MOVETO,");}
        else {print!("Path.LINETO,");}
    }
    print!("Path.CLOSEPOLY,");
    println!("]");

    println!("{}Path = Path({}Verts, {}Codes)", name, name, name);

    println!("{}Patch = patches.PathPatch({}Path, facecolor='orange', lw=2)", name, name);
    println!("ax.add_patch({}Patch)", name);

    println!("for(x,y) in {}Verts[:-1]:", name);
    println!("\tminx = min(x,minx)");
    println!("\tmaxx = max(x,maxx)");
    println!("\tminy = min(y,miny)");
    println!("\tmaxy = max(y,maxy)");
}

fn merge_inner<T>(outer: &mut Vec<P3<T>>, mut inners: Vvec<P3<T>>) -> Vec<P3<T>>
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + Copy + Default + MinMax,
    T: Into<f64> + FromF64
{
    //merge inner ring with highest x coordinate first (this one can defneitely to see the outer ring)
    inners.sort_by(|a, b| rightmost(a).partial_cmp(&rightmost(b)).unwrap_or(Ordering::Equal));
    inners.reverse();

     //merge rings one by one
    for inner in inners{
        //get rightmost point in inner ring
        let mut rightmost = (T::minv(),T::minv(),T::minv());
        let mut rightmost_index = 0;
        for (index,point) in inner.iter().enumerate(){
            if point.0 > rightmost.0 {
                rightmost = *point;
                rightmost_index = index;
            }
        }

        //calculate closest intersection with outer ring when going to the right
        let mut intersect = (T::default(),T::default(),T::default());
        let mut intersect_index = 0;
        let mut best_dis: T = T::maxv();

        let x3: f64 = rightmost.0.into();
        let y3: f64 = rightmost.1.into();
        for i in 0..outer.len(){
            let x1: f64 = outer[i].0.into();
            let y1: f64 = outer[i].1.into();
            let x2: f64 = outer[(i + 1) % outer.len()].0.into();
            let y2: f64 = outer[(i + 1) % outer.len()].1.into();

            if y2-y1 == 0.0 {continue}
            let t: f64 = (y3 - y1) / (y2 - y1);
            if t < 0.0 || t > 1.0 {continue}

            let x: f64 = x1 + t * (x2 - x1);
            let cur_dis: f64 = x - x3;
            if cur_dis<0.0 || T::from(cur_dis) >= best_dis {continue}

            best_dis = T::from(cur_dis);
            let z1: f64 = outer[i].2.into();
            let z2: f64 = outer[(i + 1) % outer.len()].2.into();
            let z: T = T::from(z1 + t * (z2 - z1));
            intersect_index = (i + 1) % outer.len();
            intersect = (T::from(x),T::from(y3),z);
        }

        let mut new_vertices= Vec::new();
        for (i,point) in outer.iter().enumerate(){
            if i == intersect_index{
                new_vertices.push(intersect);

                let mut k = rightmost_index;
                let mut step = 0;
                while step < inner.len(){
                    new_vertices.push(inner[k]);
                    k = (k+1)%inner.len();
                    step+=1;
                }

                new_vertices.push(inner[k]);
                new_vertices.push(intersect);
            }
            new_vertices.push(*point);
        }

        *outer = new_vertices.clone();
    }
    outer.to_vec()
}

fn rightmost<T>(inner: &[P3<T>]) -> T
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + Copy + MinMax
{
    let mut rightmost = T::minv();
    for (x,_,_) in inner{
        if x > &rightmost{
            rightmost = *x;
        }
    }
    rightmost
}

fn make_indices<T>(vertices: &[P3<T>], logger: &mut Logger) -> Option<Vec<u16>>
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + Copy,
    T: Into<f64> + std::fmt::Debug
{
    if vertices.len() < 3 {
        logger.log(Issue::PolyNotEnoughVertices);
        return None;
    }

    let mut remaining_polygon = VecList::new();
    remaining_polygon.reserve(vertices.len());
    let mut orig_indices = Vec::new();
    for (i,point) in vertices.iter().enumerate(){
        let x = u16::try_from(i);
        if x.is_err(){
            logger.log(Issue::OutOfIndicesBound);
            return None;
        }
        let p = PolyPoint{
            point: *point,
            reflex: false,
            ear: false,
            index: x.unwrap(),
        };
        orig_indices.push(remaining_polygon.push_back(p));
    }

    let mut i = 0;
    let clone = &remaining_polygon.clone();
    for value in remaining_polygon.iter_mut(){
        value.reflex = is_reflex(clone, orig_indices[i]);
        i+=1;
    }

    i=0;
    let clone2 = &remaining_polygon.clone();
    for value in remaining_polygon.iter_mut(){
        value.ear = is_ear(clone2, orig_indices[i]);
        i+=1;
    }

    let mut indices = Vec::new();

    let mut cur_index = orig_indices[0];

    let mut step = 0;
    while remaining_polygon.len() > 3 {
        step+=1;

        if step > vertices.len(){
            if logger.debug_panic{
                print!("[");
                for (x,_,_) in vertices{
                    print!("{:?} ", x);
                }
                print!("],[");
                for (_,y,_) in vertices{
                    print!("{:?} ", y);
                }
                print!("]");
                panic!("reeeee");
            }
            logger.log(Issue::NoEarsLeft);
            return None;
        }

        let cur = remaining_polygon.get(cur_index).unwrap();
        if !cur.ear {
            cur_index = next_index_cyclic(&remaining_polygon, cur_index);
            continue
        }

        step = 0;

        indices.push(prev_cyclic(&remaining_polygon,cur_index).index);
        indices.push(cur.index);
        indices.push(next_cyclic(&remaining_polygon,cur_index).index);

        let prev_index = prev_index_cyclic(&remaining_polygon,cur_index);
        let next_index = next_index_cyclic(&remaining_polygon, cur_index);
        remaining_polygon.remove(cur_index);

        update(&mut remaining_polygon, prev_index);
        update(&mut remaining_polygon, next_index);

        cur_index = prev_index;
    }

    for point in remaining_polygon{
        indices.push(point.index);
    }
    Some(indices)
}

fn next_cyclic<T>(polygon: &VecList<T>, i: Index<T>) -> &T{
    if let Some(y) = polygon.get_next_index(i){
        polygon.get(y).unwrap()
    }else{
        polygon.front().unwrap()
    }
}

fn prev_cyclic<T>(polygon: &VecList<T>, i: Index<T>) -> &T{
    if let Some(y) = polygon.get_previous_index(i){
        polygon.get(y).unwrap()
    }else{
        polygon.back().unwrap()
    }
}

//cyclic is very slow and stupid, but don't know how to do better with this library
fn next_index_cyclic<T>(polygon: &VecList<T>, i: Index<T>) -> Index<T>{
    if let Some(y) = polygon.get_next_index(i){
        y
    }else{
        //return the first index
        let mut indices = polygon.indices();
        indices.next().unwrap()
    }
}

//cyclic is very slow and stupid, but don't know how to do better with this library
fn prev_index_cyclic<T>(polygon: &VecList<T>, i: Index<T>) -> Index<T>{
    if let Some(y) = polygon.get_previous_index(i){
        y
    }else{
        //return the last index
        let indices = polygon.indices();
        indices.last().unwrap()
    }
}

fn update<T>(polygon: &mut VecList<PolyPoint<T>>, i: Index<PolyPoint<T>>)
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + Copy,
    T: Into<f64>
{
    let mut ear = false;
    let mut reflex = false;
    let p = polygon.get(i).unwrap();
    if !p.reflex {
        ear = is_ear(polygon, i);
        //convex points will stay convex
    }
    else{
        //reflex points might become convex
        reflex = is_reflex(polygon, i);
        //..and might become an ear
        if !reflex{
            ear = is_ear(polygon, i);
        }
    }

    let mut p_mut = polygon.get_mut(i).unwrap();
    p_mut.reflex = reflex;
    p_mut.ear = ear;
}

fn is_reflex<T>(polygon: &VecList<PolyPoint<T>>, i: Index<PolyPoint<T>>) -> bool
where
    T: Mul<Output = T> + Sub<Output = T> + PartialOrd + Copy,
    T: Into<f64>
{
    let ax = prev_cyclic(polygon,i).point.0.into();
    let ay = prev_cyclic(polygon,i).point.1.into();
    let bx = polygon.get(i).unwrap().point.0.into();
    let by = polygon.get(i).unwrap().point.1.into();
    let cx = next_cyclic(polygon,i).point.0.into();
    let cy = next_cyclic(polygon,i).point.1.into();

    (bx - ax) * (cy - by) - (cx - bx) * (by - ay) > 0.0
}

fn is_ear<T>(polygon: &VecList<PolyPoint<T>>, i: Index<PolyPoint<T>>) -> bool
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + Copy,
    T: Into<f64>
{
    //an ear is a point of a triangle with no other points inside
    let p = polygon.get(i).unwrap();
    if is_reflex(polygon,i) {return false}
    let p_prev = prev_cyclic(polygon,i);
    let p_next = next_cyclic(polygon,i);
    for node in polygon{
        if node == p_prev || node == p || node == p_next {continue}
        if is_inside_triangle(node.point, p_prev.point, p.point, p_next.point) {
            return false
        }
    }
    true
}

fn is_inside_triangle<T>(p:P3<T>,p0:P3<T>,p1:P3<T>,p2:P3<T>) -> bool
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + Copy,
    T: Into<f64>
{
    let x0 = p.0.into();
    let y0 = p.1.into();
    let x1 = p0.0.into();
    let y1 = p0.1.into();
    let x2 = p1.0.into();
    let y2 = p1.1.into();
    let x3 = p2.0.into();
    let y3 = p2.1.into();

    if  p==p0 || p==p1 || p==p2{
        return false
    }
    let denominator = (y2 - y3)*(x1 - x3) + (x3 - x2)*(y1 - y3);
    let aa = ((y2 - y3)*(x0 - x3) + (x3 - x2)*(y0 - y3)) / denominator;
    let bb = ((y3 - y1)*(x0 - x3) + (x1 - x3)*(y0 - y3)) / denominator;
    let cc = 1.0 - aa - bb;


    //epsilon is needed because of how inner and outer polygons are merged because
    //there will be two exactly equal lines in the polygon, only in reversed order
    aa >= 0.0 && aa <= 1.0 && bb >= 0.0 && bb <= 1.0 && cc >= 0.0 && cc <= 1.0
}


fn is_inside_polygon<T>(polygon: Vec<P3<T>>, p: P3<T>)-> bool
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T> + Sub<Output = T> + PartialOrd + Copy,
    T: Into<f64>
{
    //shoot a ray to the right and count how many times it intersects the polygon
    //even means outside, odd means inside
    let mut intersects = 0;
    for i in 0..polygon.len(){
        let x1 = polygon[i].0.into();
        let y1 = polygon[i].1.into();
        let x2 = polygon[(i + 1) % polygon.len()].0.into();
        let y2 = polygon[(i + 1) % polygon.len()].1.into();
        let p0 = p.0.into();
        let p1 = p.1.into();

        if y2-y1 == 0.0 {continue}
        let t = (p1 - y1) / (y2 - y1);
        if t < 0.0 || t >= 1.0 {continue}
        let x = x1 + t * (x2 - x1);
        if x<p0 {continue}

        //there was an intersection
        intersects+=1;
    }
    intersects % 2 == 1
}
